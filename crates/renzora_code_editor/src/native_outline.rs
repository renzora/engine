//! Bevy-native (ember) Outline panel — lists functions/classes in the active
//! editor tab; clicking a row jumps the editor to that line. Reuses
//! `extract_symbols`; reads `CodeEditorState`; click writes `pending_goto_line`.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_editor_framework::SplashState;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_bg, keyed_list_tokened, KeyedSnapshot};
use renzora_ember::theme::{accent, rgb, section_bg, text_muted, text_primary};

use crate::highlight::Language;
use crate::outline::{extract_symbols, OutlineSymbol, SymbolKind};
use crate::state::CodeEditorState;


#[derive(Component)]
struct GotoLine(usize);

pub fn register_native_outline(app: &mut App) {
    app.register_panel_content("outline", true, build);
    app.add_systems(Update, outline_goto_click.run_if(in_state(SplashState::Editor)));
}

fn build(commands: &mut Commands, _fonts: &EmberFonts) -> Entity {
    let list = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                flex_shrink: 0.0,
                ..default()
            },
            Name::new("outline-list"),
        ))
        .id();
    keyed_list_tokened(commands, list, outline_token, outline_snapshot);
    list
}

/// Dirty token for the outline: the active tab plus the file's path and content.
/// Hashing the content is far cheaper than re-tokenizing it, so the (parsing)
/// snapshot only runs when the file actually changes. Hashing the content (rather
/// than tracking an edit revision) means the token can never go stale.
fn outline_token(world: &World) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    if let Some(state) = world.get_resource::<CodeEditorState>() {
        state.active_tab.hash(&mut h);
        if let Some(file) = state.active_tab.and_then(|i| state.open_files.get(i)) {
            file.path.hash(&mut h);
            file.content.hash(&mut h);
        }
    }
    h.finish()
}

/// What the outline shows this frame.
enum Item {
    Note(String),
    Symbol(OutlineSymbol),
}

fn active(world: &World) -> Option<(String, Language)> {
    let state = world.get_resource::<CodeEditorState>()?;
    let file = state.active_tab.and_then(|i| state.open_files.get(i))?;
    let lang = file
        .path
        .extension()
        .and_then(|e| e.to_str())
        .map(Language::from_extension)
        .unwrap_or(Language::PlainText);
    Some((file.content.clone(), lang))
}

fn outline_snapshot(world: &World) -> KeyedSnapshot {
    let items: Vec<Item> = match active(world) {
        None => vec![Item::Note(renzora::lang::t("code.no_file"))],
        Some((content, lang)) => {
            let syms = extract_symbols(&content, lang);
            if syms.is_empty() {
                vec![Item::Note(renzora::lang::t("code.no_symbols"))]
            } else {
                syms.into_iter().map(Item::Symbol).collect()
            }
        }
    };
    let keyed: Vec<(u64, u64)> = items
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            match it {
                Item::Note(s) => (0u8, s).hash(&mut h),
                Item::Symbol(s) => (1u8, &s.name, s.line, matches!(s.kind, SymbolKind::Class)).hash(&mut h),
            }
            (i as u64, h.finish())
        })
        .collect();
    KeyedSnapshot {
        items: keyed,
        build: Box::new(move |c, f, i| match &items[i] {
            Item::Note(s) => note(c, f, s),
            Item::Symbol(s) => symbol_row(c, f, s),
        }),
    }
}

fn note(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                padding: UiRect::top(Val::Px(8.0)),
                ..default()
            },
        ))
        .id()
}

fn symbol_row(commands: &mut Commands, fonts: &EmberFonts, sym: &OutlineSymbol) -> Entity {
    let (icon, icon_color) = match sym.kind {
        SymbolKind::Function => ("\u{0192}", accent()),
        SymbolKind::Class => ("C", text_muted()),
    };
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                width: Val::Percent(100.0),
                height: Val::Px(22.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                column_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            GotoLine(sym.line + 1),
            Name::new("outline-row"),
        ))
        .id();
    bind_bg(commands, row, move |w| {
        match w.get::<Interaction>(row) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(section_bg()),
            _ => Color::NONE,
        }
    });
    let ic = commands
        .spawn((Text::new(icon), ui_font(&fonts.mono, 13.0), TextColor(rgb(icon_color))))
        .id();
    let name = commands
        .spawn((Text::new(sym.name.clone()), ui_font(&fonts.ui, 11.5), TextColor(rgb(text_primary()))))
        .id();
    let gap = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let line = commands
        .spawn((Text::new(format!("{}", sym.line + 1)), ui_font(&fonts.mono, 10.5), TextColor(rgb(text_muted()))))
        .id();
    commands.entity(row).add_children(&[ic, name, gap, line]);
    row
}

fn outline_goto_click(
    q: Query<(&Interaction, &GotoLine), Changed<Interaction>>,
    mut state: Option<ResMut<CodeEditorState>>,
) {
    let Some(state) = state.as_mut() else {
        return;
    };
    for (interaction, goto) in &q {
        if *interaction == Interaction::Pressed {
            state.pending_goto_line = Some(goto.0);
            state.goto_line_open = false;
        }
    }
}
