//! Bevy-native (ember) Code Editor panel — overrides the egui `"code_editor"`
//! body with a real bevy_ui editor: a tab bar over the open files and ember's
//! [`code_editor`] widget bound two-way to [`CodeEditorState`].
//!
//! ember owns the text buffer / caret / scroll / highlight rendering; this panel
//! supplies the document model through a [`CodeBindingSpec`] (which file is
//! shown, its text, where edits go) and a per-language [`Highlighter`] built on
//! the crate's own [`highlight_line`] tokenizer — so coloring matches the egui
//! editor exactly without ember depending back on this crate.
//!
//! Save is Ctrl+S (active file) while the panel is mounted. Files open via the
//! existing paths (`open_file` from selection / asset double-click /
//! `OpenCodeEditorFile`), by dropping a script/text asset onto the panel
//! ([`code_drop`]), or the asset browser's "Open in Code Editor" context item.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora_editor_framework::{AssetDragPayload, SplashState};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_text, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{bind_code, code_editor, CodeBindingSpec, CodeToken, Highlighter};

use crate::highlight::{highlight_line, Language, LineState, TokenKind};
use crate::state::CodeEditorState;

/// Marks the mounted panel root, so Ctrl+S only saves while the editor is open
/// and so a dropped asset over it opens in the editor.
#[derive(Component)]
struct CodeContentRoot;

/// Text/code extensions accepted by drop-to-open. Mirrors the asset browser's
/// code-editor-backed kinds.
const CODE_EXTENSIONS: &[&str] = &[
    "lua", "rhai", "rs", "py", "js", "ts", "wgsl", "glsl", "vert", "frag", "json", "toml", "yaml", "yml", "ron", "txt", "md", "html", "htm", "css", "bsn",
];

/// A tab chip → selects open_files[idx] on click.
#[derive(Component)]
struct CodeTab(usize);

/// A tab's close button → closes open_files[idx].
#[derive(Component)]
struct CodeTabClose(usize);

pub fn register_native_code_editor(app: &mut App) {
    app.register_panel_content("code_editor", false, build);
    app.add_systems(
        Update,
        (tab_click, tab_close_click, save_shortcut, code_drop).run_if(in_state(SplashState::Editor)),
    );
}

// ---------------------------------------------------------------------------
// Build
// ---------------------------------------------------------------------------

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, overflow: Overflow::clip(), ..default() },
            BackgroundColor(rgb(window_bg())),
            CodeContentRoot,
            // Detects an asset-drag release over the panel (drop-to-open).
            RelativeCursorPosition::default(),
            Name::new("native-code-editor"),
        ))
        .id();

    // Tab bar.
    let tabs = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(30.0), flex_shrink: 0.0, flex_direction: FlexDirection::Row, align_items: AlignItems::Stretch, overflow: Overflow::clip(), border: UiRect::bottom(Val::Px(1.0)), ..default() },
            BackgroundColor(rgb(header_bg())),
            BorderColor::all(rgb(border())),
            Name::new("code-tabs"),
        ))
        .id();
    keyed_list(commands, tabs, tabs_snapshot);

    // Toolbar (font size + Minimap/Whitespace), below the tab strip.
    let toolbar = crate::build_code_editor_toolbar(commands, fonts);

    // Editor body (fills); the ember editor + an empty-state note overlay.
    let body = commands
        .spawn((Node { width: Val::Percent(100.0), flex_grow: 1.0, min_height: Val::Px(0.0), flex_direction: FlexDirection::Column, position_type: PositionType::Relative, overflow: Overflow::clip(), ..default() }, Name::new("code-body")))
        .id();

    let editor = code_editor(commands, "");
    // Make the widget fill the body (it ships at a fixed 260px, bordered).
    commands.queue(move |w: &mut World| {
        if let Some(mut n) = w.get_mut::<Node>(editor) {
            n.height = Val::Auto;
            n.flex_grow = 1.0;
            n.min_height = Val::Px(0.0);
            n.border = UiRect::ZERO;
            n.border_radius = BorderRadius::ZERO;
        }
    });
    bind_code(commands, editor, binding_spec());

    let note = commands
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() },
            BackgroundColor(rgb(window_bg())),
            Name::new("code-empty-note"),
        ))
        .id();
    let note_lbl = commands.spawn((Text::new("Open a script to start editing"), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted())))).id();
    commands.entity(note).add_child(note_lbl);
    bind_display(commands, note, |w| w.get_resource::<CodeEditorState>().is_none_or(|s| s.active_tab.is_none()));

    commands.entity(body).add_children(&[editor, note]);

    // Status bar.
    let status = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(22.0), flex_shrink: 0.0, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(10.0), padding: UiRect::horizontal(Val::Px(10.0)), border: UiRect::top(Val::Px(1.0)), ..default() },
            BackgroundColor(rgb(header_bg())),
            BorderColor::all(rgb(border())),
            Name::new("code-status"),
        ))
        .id();
    let status_lbl = commands.spawn((Text::new(""), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())))).id();
    bind_text(commands, status_lbl, status_text);
    commands.entity(status).add_child(status_lbl);

    commands.entity(root).add_children(&[tabs, toolbar, body, status]);
    root
}

// ---------------------------------------------------------------------------
// Binding (host → ember)
// ---------------------------------------------------------------------------

fn active_file(s: &CodeEditorState) -> Option<&crate::state::OpenFile> {
    s.active_tab.and_then(|i| s.open_files.get(i))
}

fn binding_spec() -> CodeBindingSpec {
    CodeBindingSpec {
        // Identity = the active file's path. Excludes content (so edits don't
        // reload) and excludes the tab index (so closing another tab doesn't
        // reset the caret).
        doc_key: Box::new(|w: &World| {
            let mut h = DefaultHasher::new();
            match w.get_resource::<CodeEditorState>().and_then(active_file) {
                Some(f) => f.path.hash(&mut h),
                None => 0u64.hash(&mut h),
            }
            h.finish()
        }),
        load: Box::new(|w: &World| {
            w.get_resource::<CodeEditorState>().and_then(active_file).map(|f| f.content.clone()).unwrap_or_default()
        }),
        store: Box::new(|w: &mut World, text: &str| {
            if let Some(mut s) = w.get_resource_mut::<CodeEditorState>() {
                if let Some(i) = s.active_tab {
                    if let Some(f) = s.open_files.get_mut(i) {
                        if f.content != text {
                            f.content = text.to_string();
                            f.is_modified = true;
                        }
                    }
                }
            }
        }),
        make_highlighter: Box::new(|w: &World| -> Highlighter {
            let lang = w
                .get_resource::<CodeEditorState>()
                .and_then(active_file)
                .map(|f| Language::from_extension(&f.path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase()))
                .unwrap_or(Language::PlainText);
            Box::new(move |line: &str, st: u32| {
                let (spans, out) = highlight_line(line, lang, LineState::from_u32(st));
                let toks: Vec<CodeToken> = spans.into_iter().map(|(len, kind)| CodeToken { len: len as usize, color: kind_color(kind) }).collect();
                (toks, out.to_u32())
            })
        }),
        // The editor's live zoom (Ctrl +/-, Settings code-font size).
        font_size: Some(Box::new(|w: &World| {
            w.get_resource::<CodeEditorState>().map(|s| s.font_size).unwrap_or(14.0)
        })),
    }
}

/// Token category → bevy color, resolved from the active theme's syntax palette
/// so swapping/editing a theme recolors code live.
fn kind_color(k: TokenKind) -> Color {
    let sp = syntax_palette();
    let (r, g, b) = match k {
        TokenKind::Normal => sp.normal,
        TokenKind::Keyword => sp.keyword,
        TokenKind::Type => sp.type_,
        TokenKind::Function => sp.function,
        TokenKind::Number => sp.number,
        TokenKind::String => sp.string,
        TokenKind::Comment => sp.comment,
    };
    Color::srgb_u8(r, g, b)
}

// ---------------------------------------------------------------------------
// Tab bar (keyed_list)
// ---------------------------------------------------------------------------

fn tabs_snapshot(world: &World) -> KeyedSnapshot {
    let Some(state) = world.get_resource::<CodeEditorState>() else { return empty() };
    let active = state.active_tab;
    // (idx, name, modified, is_active)
    let files: Vec<(usize, String, bool, bool)> = state
        .open_files
        .iter()
        .enumerate()
        .map(|(i, f)| (i, f.name.clone(), f.is_modified, Some(i) == active))
        .collect();
    let items: Vec<(u64, u64)> = files
        .iter()
        .map(|(i, name, modified, is_active)| {
            let mut k = DefaultHasher::new();
            i.hash(&mut k);
            let mut h = DefaultHasher::new();
            (name, modified, is_active).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, idx| {
            let (i, name, modified, is_active) = &files[idx];
            tab_chip(c, f, *i, name, *modified, *is_active)
        }),
    }
}

fn tab_chip(commands: &mut Commands, fonts: &EmberFonts, idx: usize, name: &str, modified: bool, active: bool) -> Entity {
    let chip = commands
        .spawn((
            Node { height: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), padding: UiRect::horizontal(Val::Px(10.0)), border: UiRect::right(Val::Px(1.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(if active { rgb(window_bg()) } else { Color::NONE }),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            CodeTab(idx),
            Name::new("code-tab"),
        ))
        .id();
    let mut kids: Vec<Entity> = Vec::new();

    // Unsaved indicator (a small accent dot) before the name.
    if modified {
        let dot = commands
            .spawn((Node { width: Val::Px(7.0), height: Val::Px(7.0), border_radius: BorderRadius::all(Val::Px(4.0)), flex_shrink: 0.0, ..default() }, BackgroundColor(rgb(accent()))))
            .id();
        kids.push(dot);
    }

    let label_color = if active { text_primary() } else { text_muted() };
    let label = commands.spawn((Text::new(name.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(label_color)), bevy::text::TextLayout::no_wrap())).id();
    kids.push(label);

    // Close button (always available, even for modified tabs).
    let close = commands
        .spawn((
            Node { width: Val::Px(15.0), height: Val::Px(15.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(3.0)), flex_shrink: 0.0, ..default() },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            CodeTabClose(idx),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, "x", text_muted(), 11.0);
    commands.entity(close).add_child(ic);
    kids.push(close);

    commands.entity(chip).add_children(&kids);
    chip
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

fn tab_click(q: Query<(&Interaction, &CodeTab), Changed<Interaction>>, state: Option<ResMut<CodeEditorState>>) {
    let Some(mut state) = state else { return };
    for (interaction, tab) in &q {
        if *interaction == Interaction::Pressed {
            state.active_tab = Some(tab.0);
        }
    }
}

fn tab_close_click(q: Query<(&Interaction, &CodeTabClose), Changed<Interaction>>, state: Option<ResMut<CodeEditorState>>) {
    let Some(mut state) = state else { return };
    for (interaction, tab) in &q {
        if *interaction == Interaction::Pressed {
            state.close_tab(tab.0);
        }
    }
}

fn save_shortcut(keys: Res<ButtonInput<KeyCode>>, mounted: Query<(), With<CodeContentRoot>>, state: Option<ResMut<CodeEditorState>>) {
    if mounted.is_empty() {
        return;
    }
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if ctrl && keys.just_pressed(KeyCode::KeyS) {
        if let Some(mut state) = state {
            state.save_active();
        }
    }
}

/// Drop a script/shader/template/text asset onto the panel to open it. Mirrors
/// the viewport's native drop: on left-release of a *detached* asset drag whose
/// extension is code-editor-backed and whose cursor is over the panel, open it.
fn code_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    payload: Option<Res<AssetDragPayload>>,
    panel: Query<&RelativeCursorPosition, With<CodeContentRoot>>,
    mut commands: Commands,
    state: Option<ResMut<CodeEditorState>>,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let Some(payload) = payload else { return };
    if !payload.is_detached || !payload.matches_extensions(CODE_EXTENSIONS) {
        return;
    }
    if !panel.iter().any(|r| r.cursor_over) {
        return;
    }
    let Some(mut state) = state else { return };
    state.open_file(payload.path.clone());
    // Consume the payload so no other handler also acts on the same drop.
    commands.remove_resource::<AssetDragPayload>();
}

// ---------------------------------------------------------------------------
// Status bar
// ---------------------------------------------------------------------------

fn status_text(world: &World) -> String {
    let Some(state) = world.get_resource::<CodeEditorState>() else { return String::new() };
    let Some(f) = active_file(state) else { return String::new() };
    let lang = match Language::from_extension(&f.path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase()) {
        Language::Lua => "Lua",
        Language::Rhai => "Rhai",
        Language::Rust => "Rust",
        Language::Wgsl => "WGSL",
        Language::Python => "Python",
        Language::Shell => "Shell",
        Language::Sql => "SQL",
        Language::Json => "JSON",
        Language::Toml => "TOML",
        Language::Bsn => "BSN",
        Language::Html => "HTML",
        Language::PlainText => "Text",
    };
    let modified = if f.is_modified { "  \u{25cf} unsaved" } else { "" };
    format!("{}    {}{}", f.path.display(), lang, modified)
}

fn empty() -> KeyedSnapshot {
    KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) }
}
